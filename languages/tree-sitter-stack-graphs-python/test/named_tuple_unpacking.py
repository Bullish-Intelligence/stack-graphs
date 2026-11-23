from typing import Iterable, List, NamedTuple, Optional, Tuple

class Style:
    def update_link(self):
        pass

class Control:
    def flip(self):
        pass

class Segment(NamedTuple):
    """A piece of text."""

    text: str
    """Raw text."""
    style: Optional[Style] = None
    """An optional style."""
    control: Optional[Control] = None

    def helper(self):
        pass

    # a comment
    extra: Style = None

def assign(segment: Segment):
    text, style, control = segment
    style.update_link
    #     ^ defined: 4
    control.flip
    #       ^ defined: 8
    _, s = segment
    s.update_link
    # ^ defined: 4
    [a, b] = segment
    b.update_link
    # ^ defined: 4
    (a, b, c, d) = segment
    d.update_link
    # ^ defined: 4

def loops(segments: Iterable["Segment"], more: List[Segment]):
    for segment in segments:
        text, style, _control = segment
        style.update_link
        #     ^ defined: 4
    for text, style, control in segments:
        style.update_link
        #     ^ defined: 4
        control.flip
        #       ^ defined: 8
    for text, style, control in more:
        control.flip
        #       ^ defined: 8
    for text, style, control in [segment for segment in more]:
        pass

def plain(pair: Tuple[Style, Control], pairs: List[Tuple[Style, Control]]):
    a, b = pair
    a.update_link
    # ^ defined: 4
    b.flip
    # ^ defined: 8
    for a, b in pairs:
        b.flip
        # ^ defined: 8
