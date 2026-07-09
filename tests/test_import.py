from __future__ import annotations


def test_package_imports() -> None:
    import pomo
    from pomo.app import main

    assert callable(main)
