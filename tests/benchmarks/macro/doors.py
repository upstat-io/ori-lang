def simulate():
    doors = [False] * 100
    for p in range(1, 101):
        for idx in range(p - 1, 100, p):
            doors[idx] = not doors[idx]
    return sum(1 for d in doors if d)
total = 0
for _ in range(200):
    total += simulate()
print(total)
