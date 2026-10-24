"""Loopback-only PP-StructureV3 companion. Rust owns storage, crops and review."""
import base64
import io
import json
import logging
import os
from http.server import BaseHTTPRequestHandler, HTTPServer
from tempfile import TemporaryDirectory
from pathlib import Path

MAX_BODY = 32 * 1024 * 1024

def recognize(pipeline, payload):
    from PIL import Image
    if payload.get('fileType') != 1 or payload.get('useFormulaRecognition') is not True:
        raise ValueError('Expected a formula-enabled image request')
    encoded = payload.get('file')
    if not isinstance(encoded, str) or len(encoded) > MAX_BODY:
        raise ValueError('Invalid image size')
    data = base64.b64decode(encoded, validate=True)
    # Never accept filesystem paths or remote URLs from a caller.
    with Image.open(io.BytesIO(data)) as image:
        if image.width * image.height > 40_000_000:
            raise ValueError('Image exceeds 40 megapixels')
        with TemporaryDirectory(prefix='paperdesk-formula-') as folder:
            source = Path(folder) / 'crop.png'
            image.convert('RGB').save(source)
            pages = []
            for result in pipeline.predict(str(source), use_formula_recognition=True):
                md = result.markdown
                text = md.get('markdown_texts', md.get('markdown_text', ''))
                if not isinstance(text, str) or not text.strip():
                    raise ValueError('The model returned no text')
                if len(text.encode('utf-8')) > 200_000:
                    raise ValueError('The result exceeds 200 KB')
                pages.append({'markdown': {'text': text}})
            if not pages:
                raise ValueError('No recognition results')
            return {'errorCode': 0, 'result': {'layoutParsingResults': pages}}

def make_handler(pipeline):
    class Handler(BaseHTTPRequestHandler):
        def reply(self, code, payload):
            data = json.dumps(payload, ensure_ascii=False).encode('utf-8')
            self.send_response(code)
            self.send_header('Content-Type', 'application/json; charset=utf-8')
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        def setup(self):
            super().setup()
            self.connection.settimeout(300)
        def do_GET(self):
            if self.path != '/health':
                self.reply(404, {'error': 'Not found'})
                return
            self.reply(200, {'ready': True, 'formulas': True, 'engine': 'PP-StructureV3'})
        def do_POST(self):
            if self.headers.get('Origin') or self.headers.get('Sec-Fetch-Site') in ('cross-site', 'same-site'):
                self.reply(403, {'error': 'Browser requests are not accepted'})
                return
            if self.path != '/layout-parsing':
                self.reply(404, {'error': 'Not found'})
                return
            # Native desktop and Docker sidecar only listen inside loopback.
            try:
                length = int(self.headers.get('Content-Length', '0'))
                if length <= 0 or length > MAX_BODY:
                    raise ValueError('Invalid body size')
                body = self.rfile.read(length)
                self.reply(200, recognize(pipeline, json.loads(body)))
            except (ValueError, OSError, KeyError, TypeError):
                self.reply(400, {'errorCode': 1, 'errorMsg': 'Invalid image request'})
            except Exception:
                logging.exception("Model inference failed")
                # Do not return source text, model logs or paths to callers.
                self.reply(500, {'errorCode': 2, 'errorMsg': 'Model inference failed; check service console'})
    return Handler

def main():
    from paddleocr import PPStructureV3
    print('Loading local text, layout and formula models; first run downloads weights.', flush=True)
    pipeline = PPStructureV3(device=os.environ.get('PAPERDESK_FORMULA_DEVICE', 'cpu'),
        enable_mkldnn=False,
        use_formula_recognition=True, formula_recognition_model_name='PP-FormulaNet_plus-M',
        use_doc_orientation_classify=False, use_doc_unwarping=False,
        use_table_recognition=False, use_seal_recognition=False,
        use_region_detection=False, use_textline_orientation=False)
    # Serial inference avoids concurrent model access and unbounded memory growth.
    server = HTTPServer(('127.0.0.1', 8850), make_handler(pipeline))
    server.timeout = 300
    print('Ready: http://127.0.0.1:8850/health', flush=True)
    server.serve_forever()

if __name__ == '__main__':
    main()
