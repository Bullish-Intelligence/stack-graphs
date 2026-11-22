class A:
    def __init__(self):
        pass

class B(A):
    def __init__(self):
        super().__init__()
        #       ^ defined: 2

class C:
    pass

class D(C):
    def __init__(self):
        super().__init__()
        #       ^ defined:
        super(D, self).__init__()
        #                ^ defined:
