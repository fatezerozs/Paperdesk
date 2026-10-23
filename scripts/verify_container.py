"""Isolated Docker smoke test. Run seed, restart the QA container, then run verify."""
import io
import json
import sys
import urllib.request
from pathlib import Path
from pypdf import PdfReader

root = Path(__file__).resolve().parents[1]
base = "http://127.0.0.1:3211/api"
state_path = root / "tmp" / "docker-qa-state.json"


def api(path, body=None, method=None):
    request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(),
                                    headers={"Content-Type": "application/json", "X-Paperdesk": "1"}, method=method)
    with urllib.request.urlopen(request) as response:
        return json.load(response)


assert api("/health")["ok"]
if sys.argv[1] == "seed":
    pdf = (root / "examples" / "sample-paper.pdf").read_bytes()
    boundary = "PaperdeskDockerTestBoundary"
    body = (f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="sample-paper.pdf"\r\nContent-Type: application/pdf\r\n\r\n'.encode()
            + pdf + f"\r\n--{boundary}--\r\n".encode())
    request = urllib.request.Request(base + "/import", data=body,
                                    headers={"X-Paperdesk": "1", "Content-Type": "multipart/form-data; boundary=" + boundary})
    with urllib.request.urlopen(request) as response:
        imported = json.load(response)
    assert imported["count"] == 2
    paper_id = imported["paper"]["id"]
    with urllib.request.urlopen(base + f"/papers/{paper_id}/source") as response:
        assert response.read() == pdf
    questions = api("/questions?paper_id=" + paper_id)
    question = next(q for q in questions if q["number"] == "1")
    question.update(status="valuable", verified=True, subject="Math", answer="x = 6", explanation="Subtract 5, then divide by 2.")
    api("/questions/" + question["id"], question, "PUT")
    quiz = api("/quizzes", {"title": "Docker 中文导出验证", "count": 1, "filter": {"paper_id": paper_id}}, "POST")
    state_path.write_text(json.dumps({"quiz_id": quiz["id"], "paper_id": paper_id}), encoding="utf-8")
    print("Container PDF import, source integrity, question review, quiz generation: PASS")
else:
    state = json.loads(state_path.read_text(encoding="utf-8"))
    assert len(api("/questions?paper_id=" + state["paper_id"])) == 2
    quiz = api("/quizzes/" + state["quiz_id"])
    assert quiz["questions"][0]["answer"] == "x = 6"
    with urllib.request.urlopen(base + f"/quizzes/{state['quiz_id']}/pdf?answers=true&answer_lines=3") as response:
        assert response.headers["Content-Type"] == "application/pdf"
        pdf = response.read()
    reader = PdfReader(io.BytesIO(pdf))
    assert len(reader.pages) == 2
    assert "Docker 中文导出验证" in reader.pages[0].extract_text()
    assert "x = 6" in reader.pages[1].extract_text()
    (root / "tmp" / "pdfs" / "docker-quiz.pdf").write_bytes(pdf)
    print("Container restart persistence, snapshot, Chinese PDF and answers: PASS")
