import re, json
p = r"C:\Users\XKZ\AppData\Local\Temp\dsh-subprocess-TndKuP\dsh-subprocess-12372-2-df1769014ee6-stdout.log"
s = open(p, encoding="utf-8", errors="replace").read()
# The response is a JSON; find the array items. Try regex on each attempt object.
rows=[]
# crude: find all "wait": {"id":N,"score":X,...} pattern
for m in re.finditer(r'\{"id":(\d+).*?"score":([0-9.eE+]+).*?"authorId":"([^"]+)".*?"modelTag":"([^"]+)"', s):
    rows.append((float(m.group(2)), int(m.group(1)), m.group(3), m.group(4)))
rows.sort(reverse=True, key=lambda x: x[0])
print("total matched:", len(rows))
print("\n=== TOP 30 scores ===")
for sc,aid,auth,model in rows[:30]:
    print(f"score={sc:8.4f} attempt={aid} author={auth} model={model}")
