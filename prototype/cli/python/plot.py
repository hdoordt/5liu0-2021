import folley
import time

folley.init('/dev/ttyACM0', 3)

while True:
    s = folley.get_samples()
    print(s)
    time.sleep(1)