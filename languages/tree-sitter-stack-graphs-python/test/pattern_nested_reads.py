class A:
    pass

class B:
    content = 1

def top(event):
    match event:
        case (A() | B()):
#             ^ defined: 1
#                   ^ defined: 4
            return 1
        case [B(content=str(content))] if content:
#             ^ defined: 4
#               ^ defined: 5
#                                         ^ defined: 13
            return content
#                  ^ defined: 13

class H:
    def meth(self, event):
        match event:
            case (A() | B()):
#                 ^ defined: 1
#                       ^ defined: 4
                return 1
            case [A(), B()]:
#                 ^ defined: 1
#                      ^ defined: 4
                return 2
