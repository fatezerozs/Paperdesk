"""One bounded inference process. HTTP/health stay responsive during model execution."""
import collections
import hashlib
import json
import multiprocessing
import os
import threading


def configure_resources():
    cores = os.cpu_count() or 2
    maximum = max(1, min(4, cores - 2))
    try:
        threads = max(1, min(maximum, int(os.environ.get('PAPERDESK_OCR_THREADS', min(2, maximum)))))
    except ValueError:
        threads = min(2, maximum)
    for key in ('OMP_NUM_THREADS', 'OMP_THREAD_LIMIT', 'MKL_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'NUMEXPR_NUM_THREADS'):
        os.environ[key] = str(threads)
    os.environ['OMP_WAIT_POLICY'] = 'PASSIVE'
    os.environ['KMP_BLOCKTIME'] = '0'
    os.environ['TOKENIZERS_PARALLELISM'] = 'false'
    try:
        if os.name == 'nt':
            import ctypes
            kernel = ctypes.windll.kernel32
            kernel.GetCurrentProcess.restype = ctypes.c_void_p
            kernel.SetPriorityClass.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
            kernel.SetPriorityClass(kernel.GetCurrentProcess(), 0x4000)
        else:
            os.nice(5)
    except OSError:
        pass
    return threads


class ServiceFailure(Exception):
    def __init__(self, status, message):
        super().__init__(message)
        self.status = status


def model_worker(pipe):
    threads = configure_resources()
    try:
        from paddleocr import PPStructureV3
        from server import recognize, page_layout, pdf_layout
        pipeline = PPStructureV3(device=os.environ.get('PAPERDESK_FORMULA_DEVICE', 'cpu'),
            cpu_threads=threads, enable_mkldnn=False,
            text_recognition_batch_size=1, formula_recognition_batch_size=1,
            use_formula_recognition=True, formula_recognition_model_name='PP-FormulaNet_plus-M',
            use_doc_orientation_classify=False, use_doc_unwarping=False,
            use_table_recognition=False, use_seal_recognition=False,
            use_region_detection=False, use_textline_orientation=False)
        pipe.send(('ready', threads))
        while True:
            route, payload = pipe.recv()
            try:
                result = pdf_layout(payload) if route == '/pdf-layout' else page_layout(pipeline, payload) if route == '/page-layout' else recognize(pipeline, payload)
                pipe.send(('ok', result))
            except (ValueError, OSError, KeyError, TypeError):
                pipe.send(('error', 'Invalid image or PDF request'))
            except Exception:
                import logging
                logging.exception('Inference failed')
                pipe.send(('failure', 'Model inference failed; check service logs'))
    except (EOFError, BrokenPipeError):
        pass
    except Exception:
        import logging
        logging.exception('Model initialization failed')
    finally:
        pipe.close()


class Supervisor:
    def __init__(self, worker=model_worker, timeout=120):
        self.worker = worker
        self.timeout = timeout
        self.lock = threading.Lock()
        self.cache = collections.OrderedDict()
        self.status = {'ready': False, 'state': 'loading', 'threads': 0, 'busy': False, 'cache_hits': 0}
        self.process = None
        self.pipe = None
        self.boot_thread = threading.Thread(target=self._boot, daemon=True)
        self.boot_thread.start()

    def _dispose(self):
        if self.process:
            if self.process.is_alive():
                self.process.terminate()
            self.process.join(timeout=5)
            if self.process.is_alive():
                self.process.kill()
                self.process.join(timeout=5)
        if self.pipe:
            self.pipe.close()
        self.process = self.pipe = None

    def _boot(self):
        with self.lock:
            self.status.update(ready=False, state='loading', busy=False)
            self.cache.clear()
            try:
                context = multiprocessing.get_context('spawn')
                self.pipe, child = context.Pipe()
                self.process = context.Process(target=self.worker, args=(child,), daemon=True)
                self.process.start()
                child.close()
                if not self.pipe.poll(300):
                    raise TimeoutError('Model initialization timeout')
                kind, threads = self.pipe.recv()
                if kind != 'ready':
                    raise RuntimeError('Unexpected worker initialization')
                self.status.update(ready=True, state='ready', threads=threads, worker_pid=self.process.pid)
                print(f'Ready: one inference process, {threads} CPU threads', flush=True)
            except Exception:
                self._dispose()
                self.status.update(ready=False, state='failed')

    def run(self, route, payload):
        if not self.lock.acquire(blocking=False):
            raise ServiceFailure(429, 'Model busy or loading; retry after current task')
        try:
            if not self.status['ready']:
                raise ServiceFailure(503, 'Model not ready; check health and service log')
            key = hashlib.sha256((route + json.dumps({k:v for k,v in payload.items() if k != 'budget_seconds'}, sort_keys=True)).encode()).hexdigest()
            if key in self.cache:
                self.cache.move_to_end(key)
                self.status['cache_hits'] += 1
                return self.cache[key]
            budget = max(1.0, min(self.timeout, float(payload.get('budget_seconds', self.timeout))))
            self.status.update(busy=True, state='running')
            try:
                self.pipe.send((route, payload))
                if not self.pipe.poll(budget):
                    raise TimeoutError('Inference deadline exceeded')
                kind, result = self.pipe.recv()
            except (TimeoutError, EOFError, BrokenPipeError, OSError):
                self.status.update(ready=False, state='restarting')
                self._dispose()
                threading.Thread(target=self._boot, daemon=True).start()
                raise ServiceFailure(504, 'Inference timed out or worker exited; worker is restarting')
            if kind != 'ok':
                raise ServiceFailure(400 if kind == 'error' else 500, result)
            if len(json.dumps(result).encode()) <= 4 * 1024 * 1024:
                self.cache[key] = result
                while len(self.cache) > 8:
                    self.cache.popitem(last=False)
            return result
        finally:
            self.status['busy'] = False
            if self.status['ready']:
                self.status['state'] = 'ready'
            self.lock.release()

    def close(self):
        with self.lock:
            self._dispose()
            self.status.update(ready=False, state='stopped')
