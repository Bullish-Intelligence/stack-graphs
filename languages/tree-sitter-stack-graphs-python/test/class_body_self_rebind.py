# The right side of `X = X` in a class body names the `X` of the enclosing scope. It does not name
# the member that the same statement defines.

class Timer:
    def tick(self):
        pass

def make(x):
    return x

class Pool:
    Timer = Timer
    #       ^ defined: 4
    make = make
    #      ^ defined: 8

class Wrapped:
    Timer = staticmethod(Timer)
    #                    ^ defined: 4
    make = make(make)
    #      ^ defined: 8
    #           ^ defined: 8

# A name that the class defines before the statement wins over the enclosing scope. A later read
# reaches the rebind, and the value of the rebind leads on to the first definition.
class Earlier:
    Other = 1
    Other = Other
    #       ^ defined: 27
    Alias = Other
    #       ^ defined: 27, 28

class Shadow:
    Timer = 1
    Alias = Timer
    #       ^ defined: 34

# A read of the member outside of the class reaches the member. The value of the member leads on
# to the class that it names.
Pool.Timer
#    ^ defined: 4, 12

# A name in a method goes to the module, not to the member.
class Reader:
    Timer = Timer

    def get(self):
        return Timer
        #      ^ defined: 4
