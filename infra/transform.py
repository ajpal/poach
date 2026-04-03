import json
import pandas
import os
from pathlib import Path

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
        merged_events.append(end['time_micros'] - start['time_micros'])
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
    stripped_text = '\n'.join(lines)

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

class TimelineAggregator:
    def __init__(self, output_dir):
        self.output_dir = Path(output_dir)
        self.data_path = self.output_dir / "data.json"
        self.aggregated = {}

    def add_file(self, input_file, benchmark_name):
        """
        Process a single timeline JSON file and stage its transformed contents
        in memory.

        Args:
            input_file (str): Path to a single timeline JSON file.
            benchmark_name (str): Key to write into the aggregated data object.
        """
        input_file = Path(input_file)

        data = load_json(input_file)
        timelines = []
        for timeline in data:
            if 'evts' not in timeline:
                raise KeyError("Each JSON object must contain 'evts' array")
            events = merge_start_end_events(timeline['evts'])
            sexps = add_sexp_strs(timeline['evts'], timeline['program_text'])
            assert(len(events) == len(sexps))
            timelines.append({"events": events, "sexps": sexps})
        self.aggregated[benchmark_name] = timelines

    def save(self):
        os.makedirs(self.output_dir, exist_ok=True)
        save_json(self.data_path, self.aggregated)

class CSVAggregator:
    def __init__(self, output_dir):
        self.output_dir = Path(output_dir)
        self.data_path = self.output_dir / "data.csv"
        self.records = []

    def add_file(self, input_file):
        df = pandas.read_csv(input_file)
        self.records.append(df)

    def save(self):
        os.makedirs(self.output_dir, exist_ok=True)
        combined = pandas.concat(self.records)
        combined.to_csv(self.data_path, index=False)