import os
import sys

os.environ.setdefault("OPENCV_LOG_LEVEL", "ERROR")
os.environ.setdefault("GLOG_minloglevel", "2")

from air_control_vision.service import main

if __name__ == "__main__":
    sys.exit(main())
