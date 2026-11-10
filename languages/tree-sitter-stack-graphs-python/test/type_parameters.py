class Base:
    pass

T = "module level T"
U = "module level U"
Alias = "module level alias"

class Box[T: Base, *Ts, **P](list[T]):
#            ^ defined: 1
#                                 ^ defined: 8
    item: T
#         ^ defined: 8
    other: Ts
#          ^ defined: 8
    call: P
#         ^ defined: 8

    def get(self, key: T) -> T:
#                      ^ defined: 8
#                            ^ defined: 8
        value: T = self.item
#              ^ defined: 8
        return value

    def pick[U](self, value: U) -> U:
#                            ^ defined: 25
#                                  ^ defined: 25
        return value

def first[T](items: list[T]) -> T:
#                        ^ defined: 30
#                               ^ defined: 30
    return items[0]

def bounded[T: (int, str)](value: T) -> T:
#                                 ^ defined: 35
#                                       ^ defined: 35
    return value

def plain(value: U):
#                ^ defined: 5
    pass

print(T, U)
#     ^ defined: 4
#        ^ defined: 5

type Pair[T] = tuple[T, T]
#                    ^ defined: 48
#                       ^ defined: 48
type Simple = Base | None
#             ^ defined: 1

def use(p: Pair, s: Simple) -> Alias:
#          ^ defined: 48
#                   ^ defined: 51
#                              ^ defined: 6
    return p
