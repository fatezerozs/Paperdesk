"""Use an isolated server: python verify_scan_import.py PDF [port] [expected_count]."""
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path

pdf = Path(sys.argv[1]).read_bytes()
base = f'http://127.0.0.1:{sys.argv[2] if len(sys.argv)>2 else 3214}/api'
expected = int(sys.argv[3]) if len(sys.argv)>3 else 11
boundary = 'PaperdeskScanRegression'
body = (f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="scan-regression.pdf"\r\nContent-Type: application/pdf\r\n\r\n'.encode()
        + pdf + f'\r\n--{boundary}--\r\n'.encode())
request = urllib.request.Request(base+'/import', data=body, headers={'Content-Type':'multipart/form-data; boundary='+boundary,'X-Paperdesk':'1'})
with urllib.request.urlopen(request, timeout=200) as response: result=json.load(response)
paper=result['paper']['id']
questions=json.load(urllib.request.urlopen(base+'/questions?paper_id='+paper))
assert result['count']==expected, result
assert sorted(int(q['number']) for q in questions)==list(range(1,expected+1)), [(q['number'],q['content'][:30]) for q in questions]
assert all(q['status']=='pending' and not q['verified'] for q in questions)
assert urllib.request.urlopen(base+'/papers/'+paper+'/source').read()==pdf
def call(path, data, method='POST'):
    request=urllib.request.Request(base+path,data=json.dumps(data).encode(),headers={'Content-Type':'application/json','X-Paperdesk':'1'},method=method)
    with urllib.request.urlopen(request,timeout=200) as response:return json.load(response)
assert call('/papers/'+paper+'/recognize',{})['count']==expected
questions=json.load(urllib.request.urlopen(base+'/questions?paper_id='+paper))
q=questions[0]; q['content']='Human correction: '+q['content']; call('/questions/'+q['id'],q,'PUT')
try:
    call('/papers/'+paper+'/recognize',{})
    raise AssertionError('Edited source was overwritten')
except urllib.error.HTTPError as error: assert error.code==409
print(f'PASS: {expected} numbered candidates; page ranges: '+str([(q['number'],q['page_start'],q['page_end']) for q in questions]))
print('PASS: original bytes unchanged; re-recognition; human edits protected (409)')
