d = [1]

[d for d in d]
#^ defined: 3
#           ^ defined: 1

# The first iterable is read in the enclosing scope. A later iterable sees the targets.
[b for a in d for b in a]
#^ defined: 8
#           ^ defined: 1
#                      ^ defined: 8

# The target does not leak.
t = [e for e in d]
print(e)
#     ^ defined:

# A walrus in a comprehension binds in the enclosing function.
def walrus_in_comprehension(items):
    [(found := v) for v in items]
    return found
#          ^ defined: 20

# A generator expression has its own scope as well.
def generator(items):
    total = sum(g for g in items)
    return g
#          ^ defined:
