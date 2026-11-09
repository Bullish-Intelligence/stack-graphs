# A function sees the final bindings of the module.
x = 1
def f():
    return x
#          ^ defined: 6
x = 2

# A definition after the function shadows a builtin.
def g():
    return len, f
#          ^ defined: 13
#               ^ defined: 3
def len(a):
    pass
