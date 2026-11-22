# A name that `nonlocal` declares names the binding of the enclosing function, also when the
# enclosing function binds the name after the definition of the inner function.

def gather():
    def callback(fut):
        nonlocal finished
        finished += 1
        if finished == 3:
        #  ^ defined: 13
            return finished
            #      ^ defined: 13

    finished = 0
    return callback

def late_with_loop(n):
    def add():
        nonlocal total
        total += n
        if n:
            pass
        return total
        #      ^ defined: 26

    for i in range(n):
        total = 0
    return add

# A binding before the inner function wins over a later binding.
def early_and_late():
    seen = 1

    def mark():
        nonlocal seen
        return seen
        #      ^ defined: 31

    seen = 2
    return mark

# Without `nonlocal`, an assignment makes a local name.
def local_name():
    count = 0

    def bump():
        count = 1
        return count
        #      ^ defined: 46

    return bump
