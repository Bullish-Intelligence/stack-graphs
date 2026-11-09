def f(g):
    x = yield g
#             ^ defined: 1
    y = yield
    z = yield from g()
#                  ^ defined: 1
    w = yield x, y
#             ^ defined: 2
#                ^ defined: 4
    return x, y, z, w
#          ^ defined: 2
#             ^ defined: 4
#                ^ defined: 5
#                   ^ defined: 7
