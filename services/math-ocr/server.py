"""Loopback-only PP-StructureV3 companion. Rust owns storage, crops and review."""
import base64
import io
import json
import logging
import os
from contextlib import closing
import math
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from runtime import Supervisor, ServiceFailure, configure_resources
from tempfile import TemporaryDirectory
from pathlib import Path

MAX_BODY = 48 * 1024 * 1024

def recognize_from_hints(pipeline, image, payload):
    """Conservative reuse: uncertain or mixed inline formula lines always fall back."""
    import re
    hints=payload.get('layout_hints')
    if not isinstance(hints,dict):return None
    lines,formulas=hints.get('lines',[]),hints.get('formulas',[])
    if not lines or len(lines)>2000 or len(formulas)>64:return None
    for block in lines+formulas:
        box=block.get('bbox',[])
        if len(box)!=4 or not all(isinstance(v,(float,int)) and math.isfinite(v) for v in box):return None
        l,t,r,b=box
        if not 0<=l<r<=image.width or not 0<=t<b<=image.height:return None
    if any(not isinstance(line.get('confidence'),(float,int)) or not 0.9<=line['confidence']<=1.0 for line in lines):return None
    items=[]
    for line in lines:
        l,t,r,b=line['bbox'];area=(r-l)*(b-t);covered=False
        for formula in formulas:
            x,y,u,v=formula['bbox'];overlap=max(0,min(r,u)-max(l,x))*max(0,min(b,v)-max(t,y))/area
            if overlap>0.85:covered=True
            elif overlap>0.01:return None
        if covered and payload['useFormulaRecognition']:continue
        if payload['useFormulaRecognition'] and (re.search(r'[=<>^√∫∑∏±∞≤≥\\{}²³⁰¹⁴⁵⁶⁷⁸⁹]|[0-9]\s*[-+*/]|[A-Za-z]\s*[_^({]',line['text']) or (not formulas and re.search(r'函数|集合|证明|已知|求',line['text']))):return None
        items.append((t,l,line['text']))
    started=time.monotonic()
    if payload['useFormulaRecognition'] and formulas:
        try:model=pipeline.paddlex_pipeline.formula_recognition_pipeline.formula_recognition_model
        except AttributeError:return None
        import numpy as np
        for formula in formulas:
            l,t,r,b=formula['bbox'];crop=np.asarray(image.crop((math.floor(l),math.floor(t),math.ceil(r),math.ceil(b))))[:,:,::-1].copy()
            result=next(iter(model([crop])))
            value=result['rec_formula']
            if not isinstance(value,str) or not value.strip():return None
            items.append((t,l,'$$'+value+'$$'))
    items.sort(key=lambda item:(item[0],item[1]));text='\n'.join(item[2] for item in items)
    if not text.strip():return None
    return {'errorCode':0,'result':{'layoutParsingResults':[{'markdown':{'text':text}}]},'metrics':{'model_seconds':round(time.monotonic()-started,3),'formula_enabled':payload['useFormulaRecognition'],'reused_layout':True,'formula_regions':len(formulas)}}


def recognize(pipeline, payload):
    from PIL import Image, ImageChops
    if payload.get('fileType') != 1 or not isinstance(payload.get('useFormulaRecognition'), bool):
        raise ValueError('Expected a formula-enabled image request')
    encoded = payload.get('file')
    if not isinstance(encoded, str) or len(encoded) > MAX_BODY:
        raise ValueError('Invalid image size')
    data = base64.b64decode(encoded, validate=True)
    # Never accept filesystem paths or remote URLs from a caller.
    with Image.open(io.BytesIO(data)) as image:
        if image.width * image.height > 40_000_000:
            raise ValueError('Image exceeds 40 megapixels')
        reused=recognize_from_hints(pipeline,image.convert('RGB'),payload)
        if reused is not None:return reused
        with TemporaryDirectory(prefix='paperdesk-formula-') as folder:
            source = Path(folder) / 'crop.png'
            prepared=image.convert('RGB')
            bounds=ImageChops.difference(prepared,Image.new('RGB',prepared.size,'white')).getbbox()
            if bounds:
                l,t,r,b=bounds
                prepared=prepared.crop((max(0,l-16),max(0,t-16),min(prepared.width,r+16),min(prepared.height,b+16)))
            prepared.save(source)
            started=time.monotonic()
            pages = []
            for result in pipeline.predict(str(source), use_formula_recognition=payload['useFormulaRecognition']):
                md = result.markdown
                text = md.get('markdown_texts', md.get('markdown_text', ''))
                if not isinstance(text, str) or not text.strip():
                    raise ValueError('The model returned no text')
                if len(text.encode('utf-8')) > 200_000:
                    raise ValueError('The result exceeds 200 KB')
                pages.append({'markdown': {'text': text}})
            if not pages:
                raise ValueError('No recognition results')
            return {'errorCode': 0, 'result': {'layoutParsingResults': pages}, 'metrics':{'model_seconds':round(time.monotonic()-started,3),'formula_enabled':payload['useFormulaRecognition'],'input_width':prepared.width,'input_height':prepared.height}}

def page_layout(pipeline, payload):
    """Layout/OCR hints only: formulas are deliberately not transcribed here."""
    from PIL import Image
    data = base64.b64decode(payload['file'], validate=True)
    with Image.open(io.BytesIO(data)) as image:
        width, height = image.size
        if width * height > 40_000_000:
            raise ValueError('Image exceeds pixel limit')
        with TemporaryDirectory(prefix='paperdesk-layout-') as folder:
            source = Path(folder) / 'page.png'
            image.convert('RGB').save(source)
            result = next(iter(pipeline.predict(str(source), use_formula_recognition=False)))
            raw = result.json['res']
            blocks = [{'bbox': b['block_bbox'], 'text': b.get('block_content', ''),
                       'category': b['block_label'], 'confidence': None,
                       'order': b.get('block_order')} for b in raw['parsing_res_list']]
            ocr = raw['overall_ocr_res']
            boxes = ocr.get('rec_boxes', [])
            if not len(boxes):
                boxes = [[min(p[0] for p in poly), min(p[1] for p in poly),
                          max(p[0] for p in poly), max(p[1] for p in poly)]
                         for poly in ocr.get('rec_polys', [])]
            lines = [{'bbox': list(box), 'text': text, 'category': 'text',
                      'confidence': float(score), 'order': i}
                     for i, (box, text, score) in enumerate(zip(boxes, ocr.get('rec_texts', []), ocr.get('rec_scores', [])))]
            return {'width': width, 'height': height, 'engine': 'PP-StructureV3-layout',
                    'blocks': blocks, 'lines': lines}


def pdf_layout(payload):
    """Native text coordinates; rotated/empty pages deliberately fall back to vision."""
    import pypdfium2 as pdfium
    data = base64.b64decode(payload['file'], validate=True)
    pages = payload['pages']
    if len(data) > 32 * 1024 * 1024 or not 1 <= len(pages) <= 60:
        raise ValueError('PDF exceeds limit')
    output = []
    with pdfium.PdfDocument(data) as document:
        for spec in pages:
            index, width, height = int(spec['page']) - 1, int(spec['width']), int(spec['height'])
            if not 0 <= index < len(document) or not 0 < width <= 16000 or not 0 < height <= 16000:
                raise ValueError('Invalid page geometry')
            with closing(document[index]) as page:
                if page.get_rotation():
                    continue
                left, bottom, right, top = page.get_bbox()
                sx, sy = width / (right-left), height / (top-bottom)
                words, text, box = [], '', None
                with closing(page.get_textpage()) as tp:
                    count = tp.count_chars()
                    if count > 100000:
                        continue
                    for i in range(count):
                        char = tp.get_text_range(i, 1)
                        if not char or char.isspace():
                            if text and box:
                                words.append({'text': text, 'bbox': box, 'category': 'native_text'})
                            text, box = '', None
                            continue
                        l, b, r, t = tp.get_charbox(i)
                        if not all(math.isfinite(v) for v in [l,b,r,t]) or r <= l or t <= b:
                            continue
                        rect = [max(0, (l-left)*sx), max(0, (top-t)*sy), min(width, (r-left)*sx), min(height, (top-b)*sy)]
                        if rect[2] <= rect[0] or rect[3] <= rect[1]:
                            continue
                        # Break text-order jumps, e.g. an unseparated column transition.
                        if box and (abs(rect[1]-box[1]) > max(8, box[3]-box[1]) or rect[0] < box[0]-5):
                            words.append({'text': text, 'bbox': box, 'category': 'native_text'})
                            text, box = '', None
                        text += char
                        box = rect if box is None else [min(box[0],rect[0]), min(box[1],rect[1]), max(box[2],rect[2]), max(box[3],rect[3])]
                    if text and box:
                        words.append({'text':text,'bbox':box,'category':'native_text'})
                joined = ''.join(w['text'] for w in words)
                if len(joined.strip()) < 12 or joined.count('\ufffd') > len(joined) / 10:
                    continue
                figures = []
                scanned = False
                for number, obj in enumerate(page.get_objects(filter=[pdfium.raw.FPDF_PAGEOBJ_IMAGE], max_depth=3)):
                    if number >= 1000:
                        raise ValueError('Too many image objects')
                    l,b,r,t = obj.get_bounds()
                    rect = [max(0,(l-left)*sx),max(0,(top-t)*sy),min(width,(r-left)*sx),min(height,(top-b)*sy)]
                    if all(math.isfinite(v) for v in rect) and rect[2]>rect[0] and rect[3]>rect[1]:
                        if (rect[2]-rect[0])*(rect[3]-rect[1]) > width*height*0.6:
                            scanned = True
                        figures.append({'bbox':rect, 'text':'', 'category':'image'})
                if scanned:
                    continue
                output.append({'page':index+1, 'layout':{'width':width,'height':height,
                    'engine':'PDFium-native-text', 'lines':words, 'blocks':figures}})
    return {'pages':output}


def make_handler(pipeline):
    class Handler(BaseHTTPRequestHandler):
        def reply(self, code, payload):
            data = json.dumps(payload, ensure_ascii=False).encode('utf-8')
            try:
                self.send_response(code)
                self.send_header('Content-Type', 'application/json; charset=utf-8')
                self.send_header('Content-Length', str(len(data)))
                self.end_headers()
                self.wfile.write(data)
            except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
                pass
        def setup(self):
            super().setup()
            self.connection.settimeout(300)
        def do_GET(self):
            if self.path != '/health':
                self.reply(404, {'error': 'Not found'})
                return
            self.reply(200, {'ready': True, 'formulas': True, 'engine': 'PP-StructureV3', 'layout_api': 1, 'native_pdf': True, **pipeline.status})
        def do_POST(self):
            if self.headers.get('Origin') or self.headers.get('Sec-Fetch-Site') in ('cross-site', 'same-site'):
                self.reply(403, {'error': 'Browser requests are not accepted'})
                return
            if self.path not in ('/layout-parsing', '/page-layout', '/pdf-layout'):
                self.reply(404, {'error': 'Not found'})
                return
            # Native desktop and Docker sidecar only listen inside loopback.
            try:
                length = int(self.headers.get('Content-Length', '0'))
                if length <= 0 or length > MAX_BODY:
                    raise ValueError('Invalid body size')
                body = self.rfile.read(length)
                payload = json.loads(body)
                value = pipeline.run(self.path, payload)
                self.reply(200, value)
            except ServiceFailure as error:
                self.reply(error.status, {'errorCode':error.status, 'errorMsg':str(error)})
            except (ValueError, OSError, KeyError, TypeError):
                self.reply(400, {'errorCode': 1, 'errorMsg': 'Invalid image request'})
            except Exception:
                logging.exception("Model inference failed")
                # Do not return source text, model logs or paths to callers.
                self.reply(500, {'errorCode': 2, 'errorMsg': 'Model inference failed; check service console'})
    return Handler

def main():
    configure_resources()
    supervisor = Supervisor()
    server = ThreadingHTTPServer(('127.0.0.1', 8850), make_handler(supervisor))
    server.daemon_threads = True
    print('Service listening: http://127.0.0.1:8850/health; models loading in worker', flush=True)
    try:
        server.serve_forever()
    finally:
        server.server_close()
        supervisor.close()

if __name__ == '__main__':
    main()
