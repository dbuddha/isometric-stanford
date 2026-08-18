# Geometry kernel research oracle

OpenCV is a research oracle, not a production or ordinary-CI dependency. The
committed fixture records exact Scharr, square morphology, and eight-connected
component results from the pinned environment in `requirements.lock`.

Regenerate deliberately from the repository root:

```sh
python3 -m venv /tmp/isometric-opencv-oracle
/tmp/isometric-opencv-oracle/bin/python -m pip install -r research/geometry/requirements.lock
/tmp/isometric-opencv-oracle/bin/python research/geometry/generate_opencv_oracle.py fixtures/masks/geometry/opencv-oracle.json
```

The Rust tests consume the committed JSON and do not import OpenCV. A changed
fixture requires review of the generator, dependency versions, inputs, and Rust
kernel contract.
