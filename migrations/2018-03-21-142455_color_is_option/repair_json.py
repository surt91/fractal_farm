import re
import sqlite3

conn = sqlite3.connect('../../db.sqlite')
c = conn.cursor()

c.execute('SELECT id, json FROM fractals')
for i, json in c.fetchall():
    new_json = re.sub(
        r'[0.0,0.0,0.0]',
        r'null',
        json
    )
    c.execute("UPDATE fractals SET json = ? WHERE id = ?", (new_json, i))
    print(i)

conn.commit()
conn.close()
