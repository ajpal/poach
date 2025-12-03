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

def merge_start_end_events(timeline):
    """
    Merges pairs of start and end events into a single event with start_time_ms, end_time_ms, and total_time_ms fields.

    Args:
        timeline (list): The JSON data to process.

    Returns:
        list: The updated JSON data.
    """
    for entry in timeline:
        if 'evts' not in entry:
            raise KeyError("Each JSON object must contain 'evts' key.")

        events = entry['evts']
        merged_events = []

        for i in range(0, len(events), 2):
            start_event = events[i]
            end_event = events[i + 1]

            if start_event['evt'] != 'start' or end_event['evt'] != 'end':
                raise ValueError("Events must alternate between 'start' and 'end'.")

            merged_event = {
                'sexp_idx': start_event['sexp_idx'],
                'start_time_ms': start_event['time_ms'],
                'end_time_ms': end_event['time_ms'],
                'total_time_ms': end_event['time_ms'] - start_event['time_ms']
            }

            merged_events.append(merged_event)

        entry['evts'] = merged_events

    return timeline

def add_sexp_strs(timeline):
    """
    Adds the concrete s-expression corresponding to the sexp_id of each event.

    Args:
        timeline (list): The JSON data to process.

    Returns:
        list: The updated JSON data.
    """
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

    for entry in timeline:
        if 'program_text' not in entry or 'evts' not in entry:
            raise KeyError("Each JSON object must contain 'program_text' and 'evts' keys.")

        program_text = parse_top_level_s_expressions(entry['program_text'])
        events = entry['evts']

        for event in events:
            if 'sexp_idx' in event:
                sexp_idx = event['sexp_idx']
                if 0 <= sexp_idx < len(program_text):
                    event['sexp'] = program_text[sexp_idx]
                else:
                    raise IndexError(f"sexp_idx {sexp_idx} is out of bounds for program_text.")

    return timeline

def add_egglog_cmds(timeline):
  """
    Parses the egglog command present in each s-expression.

    Args:
        timeline (list): The JSON data to process.

    Returns:
        list: The updated JSON data.
    """
  for entry in timeline:
    events = entry['evts']

    for event in events:
      if 'sexp' not in event:
        raise KeyError("Event is missing the concrete s-expression.")
      event['cmd'] = re.search(r"[^()\s]+", event['sexp']).group()

  return timeline

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

        data = merge_start_end_events(data)
        data = add_sexp_strs(data)
        data = add_egglog_cmds(data)

        aggregated[benchmark] = data

        # save_json(output_file_path, data)
    save_json(os.path.join(output_dir, "data.json"), aggregated)