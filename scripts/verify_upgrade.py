"""Seed an isolated 0.2 QA instance; export configurable PDF fixtures."""
import json
import urllib.request
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
BASE="http://127.0.0.1:3214/api"

def api(path,body=None,method=None):
    r=urllib.request.Request(BASE+path,data=None if body is None else json.dumps(body).encode(),headers={"Content-Type":"application/json","X-Paperdesk":"1"},method=method)
    with urllib.request.urlopen(r) as response:return json.load(response)

sources=[api("/papers",{"name":name},"POST") for name in ["方程练习 · 样例","数列练习 · 样例"]]
ids=[]
for i in range(1,13):
    equation=i<=8
    q={"id":"","paper_id":sources[0 if equation else 1]["id"],"number":str(i),"content":f"解方程：2x + {i} = {3*i}。请写出计算过程，并将结果代回原方程验证。" if equation else f"等差数列首项为 {i}，公差为 2。求它的前 10 项之和，并说明计算方法。","page_start":1,"page_end":1,"subject":"数学","difficulty":1+i%5,"tags":["方程","一次方程"] if equation else ["数列","等差数列"],"status":"pending","verified":False,"answer":f"x = {i}" if equation else str(10*i+90),"explanation":"两边减去常数项，然后除以未知数系数。" if equation else "等差数列求和：n × 首项 + 公差 × n(n-1)/2。","parent_id":None,"revision":0}
    q=api("/questions",q,"POST");q.update(status="valuable",verified=True);q=api("/questions/"+q["id"],q,"PUT");ids.append(q["id"])
request={"title":"分层练习 · 方程","count":6,"filter":{"min_difficulty":2,"max_difficulty":4},"selection":{"paper_ids":[sources[0]["id"]],"tags":["方程"],"numbers":[str(i) for i in range(1,9)]},"quotas":[{"difficulty":d,"count":2} for d in [2,3,4]]}
preview=api("/quizzes/preview",request,"POST");assert preview["eligible"]==6 and preview["can_create"]
quiz=api("/quizzes",request,"POST")
for d in [2,3,4]:assert sum(q["difficulty"]==d for q in quiz["questions"])==2
out=ROOT/"output"/"pdf";out.mkdir(parents=True,exist_ok=True)
for page,columns,name in [("a5",2,"v02-a5-two-columns.pdf"),("a4",1,"v02-a4-with-answers.pdf")]:
    with urllib.request.urlopen(BASE+f"/quizzes/{quiz['id']}/pdf?answers=true&answer_lines=3&page_size={page}&columns={columns}&font_size=10.5&line_spacing=1.5&margin=15") as r:(out/name).write_bytes(r.read())
(ROOT/"tmp"/"upgrade-qa-state.json").write_text(json.dumps({"sources":sources,"ids":ids,"quiz_id":quiz["id"]}),encoding="utf-8")
print("Advanced sampling and configurable PDF exports: PASS")
