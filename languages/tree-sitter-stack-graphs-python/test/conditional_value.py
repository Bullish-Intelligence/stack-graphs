class One:
    def run(self):
        pass

class Two:
    def run(self):
        pass

def pick(flag):
    x = One() if flag else Two()
    x.run
#     ^ defined: 2, 6

class Holder:
    one = One
    two = Two

    def make(self, flag):
        cls = self.one if flag else self.two
        obj = cls()
        obj.run
#           ^ defined: 2, 6
