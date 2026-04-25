from typing import Protocol


class _BinaryWriter(Protocol):
    def write(self, data: bytes, /) -> object: ...

GAIN_STEP_DB: float

def aac_apply_gain_file(src_path: str, dst_path: str, gain_steps: int) -> int: ...
def aac_apply_gain_to_writer(src_path: str, output: _BinaryWriter, gain_steps: int) -> int: ...
