n = 0

# A parameter of a lambda does not leak.
f = lambda p: p
#             ^ defined: 4
print(p)
#     ^ defined:

# A walrus in a lambda binds in the lambda.
g = lambda: (m := 1, m)
#                    ^ defined: 10
print(m)
#     ^ defined:

# A lambda reads the module, and a parameter shadows it.
h = lambda n: n
#             ^ defined: 16
k = lambda: n
#           ^ defined: 1

# A default of a lambda is read in the enclosing scope.
def outer(q):
    return lambda r=q: r
#                   ^ defined: 22
#                      ^ defined: 23, 22
