# A read after `nonlocal x` and `x += 1` goes to the binding of the enclosing function.

def counter():
    done = 0

    def bump():
        nonlocal done
        done += 1
        if done == 3:
        #  ^ defined: 4
            return done
            #      ^ defined: 4

    bump()
    return done
    #      ^ defined: 4

def condition():
    total = 0

    def add(n):
        nonlocal total
        total += n
        if n:
            return total
            #      ^ defined: 19
        return total
        #      ^ defined: 19

    return add

# Control: without `nonlocal`, the augmented assignment defines a local.
def local_copy():
    seen = 0

    def bump():
        seen += 1
        return seen
        #      ^ defined: 37

    return bump
