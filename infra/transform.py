import json
import re
import os
import glob

def load_json(path):
  with open(path, 'r') as file:
    data = json.load(file)
  return data

def save_json(path, data):
  os.makedirs(os.path.dirname(path), exist_ok=True)
  with open(path, 'w') as file:
    json.dump(data, file, indent=4)

def merge_start_end_events(timeline_events):
    merged_events = []
    for i in range(0, len(timeline_events), 2):
        start = timeline_events[i]
        end = timeline_events[i + 1]
        
        if start['evt'] != 'start' or end['evt'] != 'end':
            raise ValueError("Events must alternate between start and end")
        
        assert(start['sexp_idx'] == end['sexp_idx'])
        merged_events.append(end['time_ms'] - start['time_ms'])
    return merged_events

def strip_comments(line):
    in_quote = False
    result = []
    for ch in line:
        if ch == '"':
            in_quote = not in_quote
            result.append(ch)
        elif ch == ';' and not in_quote:
            break
        else:
            result.append(ch)
    return ''.join(result).rstrip()

def parse_top_level_s_expressions(program_text):
    # Remove comments and blanks
    lines = [strip_comments(line) for line in program_text.splitlines()]
    lines = [line.strip() for line in lines if line.strip() != ""]
    stripped_text = ''.join(lines)

    stack = []
    current = ''
    expressions = []

    for char in stripped_text:
        if char == '(':
            if not stack:
                current = ''
            stack.append('(')
            current += char
        elif char == ')':
            if stack:
                stack.pop()
            current += char
            if not stack:
                expressions.append(current)
        else:
            current += char

    return expressions

def add_sexp_strs(timeline_events, program_text):
    parsed = parse_top_level_s_expressions(program_text)
    sexps = []
    # Count by 2 to account for start and end events
    for i in range(0, len(timeline_events), 2):
        event = timeline_events[i]
        if 'sexp_idx' not in event:
            raise KeyError("Event missing sexp_idx")
        sexp_idx = event['sexp_idx']
        if 0 <= sexp_idx < len(parsed):
            sexps.append(parsed[sexp_idx])
        else:
            raise IndexError("sexp_idx out of bounds")
    return sexps

def poison(data):
    data.append({"poison": True})

def transform(input_dir, output_dir):
    """
    Processes all JSON files in the input directory, applying each transformation in order,
    and writes the results to the output directory.

    Args:
        input_dir (str): Path to the input directory containing JSON files.
        output_dir (str): Path to the output directory to save processed JSON files.
    """
    os.makedirs(output_dir, exist_ok=True)

    pattern = os.path.join(input_dir, "*/*/timeline.json")
    benchmark_names = [f.removeprefix(f"{input_dir}/") for f in glob.glob(pattern) if os.path.isfile(f)]
    save_json(os.path.join(output_dir, "list.json"), benchmark_names)

    aggregated = {}

    for benchmark in benchmark_names:
        input_file_path = input_dir / benchmark
        output_file_path = os.path.join(output_dir, benchmark)

        data = load_json(input_file_path)
        timelines = []
        for timeline in data:
            if 'evts' not in timeline:
                raise KeyError("Each JSON object must contain 'evts' array")
            events = merge_start_end_events(timeline['evts'])
            sexps = add_sexp_strs(timeline['evts'], timeline['program_text'])
            assert(len(events) == len(sexps))
            timelines.append({"events": events, "sexps": sexps})
        aggregated[benchmark] = timelines

    save_json(os.path.join(output_dir, "data.json"), aggregated)