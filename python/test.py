
import sys
import time

while True:
    data = sys.stdin.buffer.read(1000)
    print(data)
    time.sleep(0.5)