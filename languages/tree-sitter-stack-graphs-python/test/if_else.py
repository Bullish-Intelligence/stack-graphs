# Every branch of an `if` statement with an `else` clause binds the name, so the binding before
# the statement is not reachable after it.
a = 1
if cond:
    a = 2
else:
    a = 3
print(a)
#     ^ defined: 5,7

# One branch does not bind the name. The binding before the statement stays reachable.
b = 1
if cond:
    b = 2
else:
    c = 3
print(b)
#     ^ defined: 12,14

# An `if` statement with `elif` and `else` clauses.
d = 1
if cond:
    d = 2
elif other:
    e = 3
else:
    d = 4
print(d)
#     ^ defined: 21,23,27

# A nested `if` statement that rebinds in a loop. The inner binding is conditional.
f = 1
if cond:
    f = 2
else:
    for item in items:
        if item:
            f = 3
print(f)
#     ^ defined: 32,34,38

# An `if` statement without `else` keeps the binding before it.
g = 1
if cond:
    g = 2
print(g)
#     ^ defined: 43,45

# An `if` statement with `elif` and no `else`.
h = 1
if cond:
    h = 2
elif other:
    h = 3
print(h)
#     ^ defined: 50,52,54

# A function that returns in one branch.
def function(i, j):
    i = 1
    if cond:
        i = 2
    elif other:
        j = 3
    else:
        return None
    return i
    #      ^ defined: 60,62

