def task_ready(request):
    pass

def make(base, task_ready):
    class Request(base):
        def on_success(self):
            task_ready(self)
            # ^ defined: 4

    return Request

# Known gap: with a default value (`task_ready=task_ready`) the graph reaches the parameter and
# the module-level name. The reconciler of Scippy keeps the parameter. A test of the graph alone
# cannot say so.
