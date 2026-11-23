# A variable annotation in a function does not see the hoisted locals of the function.

class Foo: pass
class Bar: pass

def local_import_later():
    x: Foo | Bar = None
#      ^ defined: 3
#            ^ defined: 4
    from other import Foo
    return x

def local_class_before():
    class Local: pass
    y: Local = None
#      ^ defined: 14
    return y

def parameter_type(Param):
    z: Param = None
#      ^ defined: 19
    return z
