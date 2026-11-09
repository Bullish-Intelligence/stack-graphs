# An inner binding shadows a binding of the same name in an outer scope or in the builtins.

def process():
    pass

def input(): pass
class dict: pass

def param_over_builtin(id):
    return id
    #      ^ defined: 9

def param_over_module(process):
    process()
    #  ^ defined: 13

def typed_default_star_params(type: int, hash=1, *iter, **min):
    return type, hash, iter, min
    #      ^ defined: 17
    #            ^ defined: 17
    #                  ^ defined: 17
    #                        ^ defined: 17

class Holder:
    def method(self, process, id):
        process()
        #  ^ defined: 25
        return id
        #      ^ defined: 25

lam = lambda process, id: (process, id)
#                          ^ defined: 31
#                                   ^ defined: 31

def function_over_builtin():
    return input
    #      ^ defined: 6

def class_over_builtin():
    return dict
    #      ^ defined: 7

def nested_over_builtin():
    def len(): pass
    return len
    #      ^ defined: 44

def local_over_builtin():
    id = 1
    return id
    #      ^ defined: 49

def later_module_definition():
    return later
    #      ^ defined: 57

def later():
    pass

def conditional_binding(flag):
    if flag:
        input = 1
    return input
    #      ^ defined: 6, 62

x = 1
x = 2
print(x)
#     ^ defined: 67

class ClassBody:
    process = 1
    print(process)
    #     ^ defined: 72

print(process)
#     ^ defined: 3
