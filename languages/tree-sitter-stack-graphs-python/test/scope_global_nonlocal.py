total = 0

class Box:
    total = 10

    # A method that declares `global` reaches the module, not the class body.
    def bump(self):
        global total
        total += 1
        return total
#              ^ defined: 1

    # Without `global`, a method does not see the class body.
    def plain(self):
        return total
#              ^ defined: 1

# `nonlocal` skips a class between two functions.
def outer():
    count = 0

    class Mid:
        count = 5

        def inner(self):
            nonlocal count
            count += 1
            return count
#                  ^ defined: 20

    return Mid

# `global` in a nested function reaches the module, not the enclosing function.
def enclosing():
    total = 5

    def inner():
        global total
        return total
#              ^ defined: 1

    return inner

# A comprehension in a function that declares `global` sees the module name.
def with_comprehension():
    global total
    return [total for _ in range(3)]
#           ^ defined: 1
