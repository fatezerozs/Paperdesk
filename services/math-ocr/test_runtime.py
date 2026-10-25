import threading
import time
import unittest
import urllib.request
import json
from http.server import ThreadingHTTPServer
from runtime import Supervisor, ServiceFailure
from server import make_handler


def fake_worker(pipe):
    pipe.send(('ready', 2))
    count = 0
    try:
        while True:
            _, payload = pipe.recv()
            time.sleep(payload.get('delay', 0))
            count += 1
            pipe.send(('ok', {'count': count}))
    except EOFError:
        pass


class RuntimeTests(unittest.TestCase):
    def ready(self, supervisor):
        end = time.monotonic()+10
        while not supervisor.status['ready'] and time.monotonic()<end:
            time.sleep(0.02)
        self.assertTrue(supervisor.status['ready'])

    def test_single_worker_cache_health_and_busy_rejection(self):
        supervisor = Supervisor(worker=fake_worker)
        server = None
        try:
            self.ready(supervisor)
            first = supervisor.run('/page-layout', {'file':'test'})
            second = supervisor.run('/page-layout', {'file':'test'})
            self.assertEqual(first, second)
            self.assertEqual(supervisor.status['cache_hits'], 1)
            server = ThreadingHTTPServer(('127.0.0.1', 0), make_handler(supervisor))
            threading.Thread(target=server.serve_forever, daemon=True).start()
            errors=[]
            def work():
                try:supervisor.run('/page-layout', {'file':'other','delay':0.8})
                except Exception as error:errors.append(error)
            worker=threading.Thread(target=work)
            worker.start()
            end=time.monotonic()+3
            while not supervisor.status['busy'] and time.monotonic()<end:time.sleep(0.01)
            start=time.monotonic()
            with urllib.request.urlopen(f'http://127.0.0.1:{server.server_port}/health',timeout=2) as response:
                health=json.load(response)
            self.assertTrue(health['busy'])
            self.assertLess(time.monotonic()-start,0.7)
            with self.assertRaises(ServiceFailure) as result:
                supervisor.run('/page-layout', {'file':'third'})
            self.assertEqual(result.exception.status,429)
            worker.join();self.assertFalse(errors)
        finally:
            if server:server.shutdown();server.server_close()
            supervisor.close()

    def test_timeout_terminates_and_replaces_worker(self):
        supervisor=Supervisor(worker=fake_worker,timeout=1)
        try:
            self.ready(supervisor)
            original=supervisor.process.pid
            with self.assertRaises(ServiceFailure) as result:
                supervisor.run('/page-layout',{'delay':4})
            self.assertEqual(result.exception.status,504)
            self.ready(supervisor)
            self.assertNotEqual(original,supervisor.process.pid)
            self.assertEqual(supervisor.run('/page-layout',{'delay':0})['count'],1)
        finally:supervisor.close()

if __name__=='__main__':unittest.main()
