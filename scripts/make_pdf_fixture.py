"""Generate a tiny, reproducible PDF fixture; not required to run Paperdesk."""
from pathlib import Path

lines = ["Math practice - PDF import example", "1. Solve: 2x + 5 = 17.", "2. Compute the sum of the first 10 positive integers."]
stream = "BT /F1 14 Tf 50 760 Td " + " 0 -40 Td ".join(f"({line}) Tj" for line in lines) + " ET"
objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    f"<< /Length {len(stream)} >>\nstream\n{stream}\nendstream",
]
pdf = bytearray(b"%PDF-1.4\n")
offsets = []
for i, obj in enumerate(objects, 1):
    offsets.append(len(pdf))
    pdf.extend(f"{i} 0 obj\n{obj}\nendobj\n".encode("ascii"))
xref = len(pdf)
pdf.extend(b"xref\n0 6\n0000000000 65535 f \n")
for offset in offsets:
    pdf.extend(f"{offset:010} 00000 n \n".encode("ascii"))
pdf.extend(f"trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode("ascii"))
output = Path(__file__).resolve().parents[1] / "examples" / "sample-paper.pdf"
output.write_bytes(pdf)
print(output)
