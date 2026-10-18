#!/usr/bin/env python3
"""Turn the recorded terminal sessions in shots/*.txt into the README image layouts.

Record the sessions first (see CONTRIBUTING.md), then run: python3 render.py
"""
import html, pathlib, re

HERE = pathlib.Path(__file__).parent
PROMPT = "~/faber-demo $ "


def colour(line):
    line = html.escape(line)
    line = re.sub(r'(&quot;[^&]*?&quot;)(\s*:)', r'<span class="k">\1</span>\2', line)
    line = re.sub(r'(:\s*)(&quot;.*?&quot;)', r'\1<span class="s">\2</span>', line)
    line = re.sub(r'(:\s*)(-?\d+)(,?)$', r'\1<span class="n">\2</span>\3', line)
    return line


def terminal(name, skip=0):
    out = []
    for line in (HERE / "shots" / f"{name}.txt").read_text().splitlines()[skip:]:
        if line.startswith("\x01"):
            out.append(f'<span class="p">{PROMPT}</span><span class="c">{html.escape(line[1:])}</span>')
        elif line.startswith("\x02"):
            out.append(f'<span class="c">{html.escape(line[1:])}</span>')
        elif line.startswith("JOB "):
            out.append(f'<span class="h">{html.escape(line)}</span>')
        else:
            out.append(colour(line))
    return "\n".join(out)


def window(title, body, style, cls="win"):
    return (f'<div class="{cls}" style="{style}"><div class="bar"><i></i><i></i><i></i>'
            f"<span>{title}</span></div><pre>{body}</pre></div>")


def page(name, *windows):
    doc = ('<!doctype html><html><head><meta charset="utf-8"><link rel="stylesheet" href="style.css">'
           "</head><body>\n" + "\n".join(windows) + "\n</body></html>\n")
    (HERE / f"{name}.html").write_text(doc)


page("hero",
     window("bash · compile and run C", terminal("compile"), "left:56px;top:52px;width:900px"),
     window("bash · hostile jobs", terminal("contained"), "right:52px;bottom:64px;width:1010px", "win small"))
page("contained",
     window("bash · hostile jobs", terminal("contained"), "width:1480px", "win big"))
page("judge",
     window("bash · one compile, three test cases in parallel", terminal("judge"), "width:1300px", "win big"))
page("guardrails",
     window("bash · what the API refuses", terminal("guardrails"), "width:1420px", "win big"))
