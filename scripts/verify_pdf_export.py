"""End-to-end PDF fixture export against an isolated test instance (port 3213)."""
import json
import urllib.request
from pathlib import Path

base = "http://127.0.0.1:3213/api"


def api(path, body=None, method=None):
    request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                    headers={"Content-Type": "application/json", "X-Paperdesk": "1"}, method=method)
    with urllib.request.urlopen(request) as response:
        return json.load(response)


question = api("/questions", {
    "id": "", "paper_id": None, "number": "1", "content": "解方程：2x + 5 = 17。",
    "page_start": 1, "page_end": 1, "subject": "数学", "difficulty": 1, "tags": ["方程"],
    "status": "pending", "verified": False, "answer": "x = 6",
    "explanation": "两边同时减去 5，得到 2x = 12；再除以 2，得到 x = 6。代回原式：2×6+5=17，成立。",
    "parent_id": None, "revision": 0,
}, "POST")
question.update(status="valuable", verified=True)
api("/questions/" + question["id"], question, "PUT")
quiz = api("/quizzes", {"title": "示例测验 · 一元一次方程", "count": 1, "filter": {"subject": "数学"}}, "POST")
output = Path(__file__).resolve().parents[1] / "output" / "pdf"
output.mkdir(parents=True, exist_ok=True)
for answers, name, lines in [(False, "sample-student.pdf", 6), (True, "sample-with-answers.pdf", 3)]:
    url = f"{base}/quizzes/{quiz['id']}/pdf?answers={str(answers).lower()}&answer_lines={lines}"
    with urllib.request.urlopen(url) as response:
        assert response.headers["Content-Type"] == "application/pdf"
        (output / name).write_bytes(response.read())
print("Student and teacher PDF exports downloaded successfully.")
