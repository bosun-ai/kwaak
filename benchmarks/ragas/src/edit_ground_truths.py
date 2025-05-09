import sys
import json
import hashlib
from pathlib import Path
from fastapi import FastAPI, Request, HTTPException, Response
from fastapi.responses import HTMLResponse
from fastapi.templating import Jinja2Templates
from starlette.status import HTTP_204_NO_CONTENT
from contextlib import asynccontextmanager
import asyncio
from fastapi.staticfiles import StaticFiles
import logging

BASE_DIR = Path(__file__).parent

class RagasData:
    def __init__(self, input_path, output_path):
        self.input_path = input_path
        self.output_path = output_path
        self.input_data = []
        self.output_data = []
        self.question_hashes = []
        self.load()
    
    def load(self):
        with open(self.input_path, "r") as f:
            self.input_data = json.load(f)
            if not isinstance(self.input_data, list):
                raise ValueError(f"Input file {self.input_path} must be a list of dicts.")
            logging.info(f"Loaded input_data with {len(self.input_data)} items from {self.input_path}")
        with open(self.output_path, "r") as f:
            self.output_data = json.load(f)
            if not isinstance(self.output_data, list):
                raise ValueError(f"Output file {self.output_path} must be a list of dicts.")
            logging.info(f"Loaded output_data with {len(self.output_data)} items from {self.output_path}")

        # Both input_data and output_data are now lists of dicts
        print(f"First 2 input_data items: {self.input_data[:2]}")
        print(f"First 2 output_data items: {self.output_data[:2]}")
        all_questions = set()
        skipped_input = 0
        for v in self.input_data:
            if isinstance(v, dict) and "question" in v:
                all_questions.add(v["question"])
            else:
                skipped_input += 1
        skipped_output = 0
        for v in self.output_data:
            if isinstance(v, dict) and "question" in v:
                all_questions.add(v["question"])
            else:
                skipped_output += 1
        print(f"Found {len(all_questions)} unique questions. Skipped {skipped_input} input and {skipped_output} output items without 'question' key.")
        self.question_hashes.clear()
        print("Question hashes in loader:")
        for q in sorted(all_questions):
            h = question_hash(q)
            print(f"  {h}: {q!r}")
            self.question_hashes.append((h, q))
        return self

    def save(self):
        with open(self.output_path, "w") as f:
            json.dump(self.output_data, f, indent=2, ensure_ascii=False)
        return self


# Utility functions
def question_hash(question: str) -> str:
    return hashlib.sha256(question.encode("utf-8")).hexdigest()

app = FastAPI()
templates = Jinja2Templates(directory=str(BASE_DIR / "templates"))

input_path = None
output_path = None

def main():
    import uvicorn
    
    if len(sys.argv) < 3:
        print("Usage: edit-ground-truths <input_json> <output_json>")
        sys.exit(1)
        
    # Set global paths so they're available to the load_data function
    global input_path, output_path
    input_path = sys.argv[1]
    output_path = sys.argv[2]
    
    # Print some info
    print(f"Starting RAGAS Ground Truth Editor")
    print(f"Input file: {input_path}")
    print(f"Output file: {output_path}")
    print("Loading data...")
    
    # Load data once to validate files before starting server
    data = load_data()
    print(f"Loaded {len(data.question_hashes)} questions")
    print("Starting web server at http://127.0.0.1:8000")
    
    # Run the FastAPI app with Uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8000)

if __name__ == "__main__":
    main()

def load_data():
    return RagasData(input_path, output_path)

@app.get("/", response_class=HTMLResponse)
def index(request: Request):
    data = load_data()
    return templates.TemplateResponse(request, "index.html", {"questions": data.question_hashes})

@app.get("/questions/{qhash}", response_class=HTMLResponse)
def question_view(request: Request, qhash: str):
    data = load_data()
    # Find the question text
    qtext = None
    for h, q in data.question_hashes:
        if h == qhash:
            qtext = q
            break
    if not qtext:
        raise HTTPException(404, "Question not found")
    # Find answer and ground_truths
    input_item = None
    output_item = None
    for v in data.input_data:
        if isinstance(v, dict) and v.get("question") == qtext:
            input_item = v
            break
    for v in data.output_data:
        if isinstance(v, dict) and v.get("question") == qtext:
            output_item = v
            break
    answer = input_item.get("answer", "") if input_item else ""
    ground_truths = output_item.get("ground_truths", [""]) if output_item else [""]
    # For navigation
    idx = [i for i, (h, _) in enumerate(data.question_hashes) if h == qhash]
    next_hash = data.question_hashes[idx[0]+1][0] if idx and idx[0]+1 < len(data.question_hashes) else None
    return templates.TemplateResponse(
        request,
        "question.html",
        {
            "qhash": qhash,
            "question": qtext,
            "answer": answer,
            "ground_truths": "\n".join(ground_truths),
            "next_hash": next_hash
        }
    )

@app.put("/questions/{qhash}")
def update_ground_truth(qhash: str, request: Request):
    async def inner():
        data = load_data()
        request_data = await request.json()
        new_gts = request_data.get("ground_truths", "")
        # Find the question text
        qtext = None
        for h, q in data.question_hashes:
            if h == qhash:
                qtext = q
                break
        if not qtext:
            raise HTTPException(404, "Question not found")
        # Update output_data
        for v in data.output_data:
            if isinstance(v, dict) and v.get("question") == qtext:
                # Preserve empty lines by only stripping non-empty lines
                v["ground_truths"] = [gt.strip() if gt.strip() else "" for gt in new_gts.split("\n")]
                break
        else:
            # Not present, add
            # Preserve empty lines by only stripping non-empty lines
            data.output_data.append({"question": qtext, "ground_truths": [gt.strip() if gt.strip() else "" for gt in new_gts.split("\n")]})
        data.save()
        return Response(status_code=HTTP_204_NO_CONTENT)
    return asyncio.run(inner())

# Mount static if needed
STATIC_DIR = BASE_DIR / "static"
if STATIC_DIR.exists():
    app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")
