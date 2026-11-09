x = 1
y = [10, 20]

[x for x in y]
#^ defined: 4
#           ^ defined: 2

{x: x for x in y}
#^ defined: 8
#   ^ defined: 8

{x for x in y}
#^ defined: 12

list(x for x in y)
#    ^ defined: 15

# The variable of a comprehension does not leak into the enclosing scope.
print(x)
#     ^ defined: 1

# A variable of the enclosing scope is visible in a comprehension.
[x + y for x in y]
#^ defined: 23
#    ^ defined: 2

def f(z):
    return [z + w for w in z]
#           ^ defined: 27
#               ^ defined: 28
#                          ^ defined: 27

[(a, b) for a in y for b in a]
# ^ defined: 33
#    ^ defined: 33
#                           ^ defined: 33
