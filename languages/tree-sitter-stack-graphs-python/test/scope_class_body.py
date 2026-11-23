x = 1

class Outer:
    y = 2
    z = y
#       ^ defined: 4
    w = x
#       ^ defined: 1

    def method(self):
        a = y
#           ^ defined:
        b = x
#           ^ defined: 1
        return a, b

    class Inner:
        q = y
#           ^ defined:
        r = x
#           ^ defined: 1

# A function nested in a class body sees the enclosing function, not the class names.
def make():
    local = 1

    class InFunc:
        v = local
#           ^ defined: 25
        member = 2

        def m(self):
            return local, member
#                  ^ defined: 25
#                         ^ defined:

    return InFunc

# A name that the class binds later does not reach an earlier read in the class body.
late = 1

class Order:
    first = late
#           ^ defined: 40
    late = 2
    after = late
#           ^ defined: 45
