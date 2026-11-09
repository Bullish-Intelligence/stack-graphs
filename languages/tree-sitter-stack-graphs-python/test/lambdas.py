y = x = 1

no_params = lambda: y
            #       ^ defined: 1

sorted([1, 2, 3], key=lambda y: x + y)
                      #         ^ defined: 1
                      #             ^ defined: 6

def uses_default(fn=lambda: 1):
    fn()
#   ^ defined: 10

# A lambda sees the bindings of the enclosing function. A parameter of the lambda shadows them.
def enclosing(a, b=2):
    c = 3
    f = lambda: a + c
    #           ^ defined: 15
    #               ^ defined: 16
    g = lambda a: a + b
    #             ^ defined: 20
    #                 ^ defined: 15
    h = lambda z: lambda w: z + w + a
    #                       ^ defined: 23
    #                           ^ defined: 23
    #                               ^ defined: 15
    return f, g, h

# A lambda inside a method sees the method parameters and `self`.
class Holder:
    def method(self, key):
        return sorted([], key=lambda item: item + key + self.method)
        #                                         ^ defined: 31
        #                                                    ^ defined: 31

