def outer(a):
    def inner(b):
        pass

    inner(1)
    # ^ defined: 2

# A function of an enclosing function wins over the same name of the builtins and the module.
def enclosing():
    def format(x):
        pass

    def inner(items):
        return format(items)
        #      ^ defined: 10
        # A lambda in the function sees the same function.
        return lambda: format(1)
        #              ^ defined: 10

def format(x):
    pass

