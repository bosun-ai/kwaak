import tempfile
import shutil
import json
from pathlib import Path
import pytest
from fastapi.testclient import TestClient
import sys
import os

# Ensure src is in sys.path for import
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))
from edit_ground_truths import app, load_data, RagasData, question_hash

@pytest.fixture
def temp_input_output_files(tmp_path):
    # Use real dataset structure for input and output
    base = Path(__file__).parent.parent
    in_path = base / "datasets/kwaak_answers.json"  # This is a list of dicts with 'question' key
    out_path = base / "datasets/kwaak.json"         # Output file, also a list of dicts
    tmp_in = tmp_path / "input.json"
    tmp_out = tmp_path / "output.json"
    shutil.copy(in_path, tmp_in)
    shutil.copy(out_path, tmp_out)
    return tmp_in, tmp_out

@pytest.fixture
def test_client(monkeypatch, temp_input_output_files):
    # override load_data
    monkeypatch.setattr("edit_ground_truths.load_data", lambda: RagasData(temp_input_output_files[0], temp_input_output_files[1]))
    return TestClient(app)


def test_index_lists_questions(test_client):
    resp = test_client.get("/")
    assert resp.status_code == 200
    assert "Questions" in resp.text
    # Should list at least one question link
    assert resp.text.count("/questions/") >= 1


def test_question_view_and_update(test_client, temp_input_output_files):
    # Load a question from input
    tmp_in, tmp_out = temp_input_output_files
    with open(tmp_in) as f:
        data = json.load(f)
    print(f"Loaded input file type: {type(data)}, length: {len(data) if hasattr(data, '__len__') else 'N/A'}")
    if not isinstance(data, list) or not data:
        pytest.skip("Input file is empty or not a list of questions.")
    # Ensure all items are dicts with 'question'
    filtered = [item for item in data if isinstance(item, dict) and 'question' in item]
    if not filtered:
        pytest.skip("No valid question dicts in input file.")
    first_item = filtered[0]
    qtext = first_item["question"]
    qhash = question_hash(qtext)
    print(f"Test requesting question: {qtext!r} with hash {qhash}")
    # GET question view
    resp = test_client.get(f"/questions/{qhash}")
    assert resp.status_code == 200
    assert qtext in resp.text
    # PUT update ground truths
    new_gts = "Updated ground truth 1\nUpdated ground truth 2"
    put_resp = test_client.put(f"/questions/{qhash}", json={"ground_truths": new_gts})
    assert put_resp.status_code == 204
    # Output file should be updated
    with open(tmp_out) as f:
        out_data = json.load(f)
    found = False
    for v in out_data:
        if isinstance(v, dict) and v.get("question") == qtext:
            assert v["ground_truths"] == ["Updated ground truth 1", "Updated ground truth 2"]
            found = True
    assert found


def test_next_question_link(test_client, temp_input_output_files):
    # Load all questions
    tmp_in, _ = temp_input_output_files
    with open(tmp_in) as f:
        data = json.load(f)
    questions = [v["question"] for v in data if isinstance(v, dict) and "question" in v]
    if len(questions) < 2:
        pytest.skip("Need at least two questions for next link test")
    qhash = question_hash(questions[0])
    resp = test_client.get(f"/questions/{qhash}")
    assert resp.status_code == 200
    # Should have a link to next question
    assert "Next Question" in resp.text
