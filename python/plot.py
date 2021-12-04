import numpy as np
import matplotlib.pyplot as plt
from sys import stdin
from collections import deque
import re

data_mic1 = deque(maxlen = 1000)
data_mic2 = deque(maxlen = 1000)
data_mic3 = deque(maxlen = 1000)
data_mic4 = deque(maxlen = 1000)

fig, axes = plt.subplots(4)

cnt = 0

total = 0

for line in stdin:
    matches = re.match('(-?\d+),(-?\d+),(-?\d+),(-?\d+)\n', line)
    # print(line)
    # print(matches[1])
    data_mic1.append(int(matches[1]))
    data_mic2.append(int(matches[2]))
    data_mic3.append(int(matches[3]))
    data_mic4.append(int(matches[4]))

    cnt += 1
    total +=1

    if cnt == 50:
        axes[0].clear()
        # axes[1].clear()
        # axes[2].clear()
        # axes[3].clear()
        axes[0].plot(data_mic1, label = "mic1")
        
        # axes[1].plot(data_mic2, label = "mic2")
        
        # axes[2].plot(data_mic3, label = "mic3")
        
        # axes[3].plot(data_mic4, label = "mic4")
        
        print("plot! " + str(total))
        cnt  = 0
        plt.pause(0.001)
    

plt.show()
