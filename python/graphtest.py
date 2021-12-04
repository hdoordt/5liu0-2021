from typing import Iterable
from pyqtgraph.Qt import QtGui, QtCore
import pyqtgraph as pg

import collections
import random
import time
import math
import numpy as np
import sys 

class InputBuffer():

    def __init__(self, maxlen=200000):
        self.maxlen = maxlen
        self.buf = []
        self.start = 0
        self.end = 0

    def extend(self, items: Iterable):
        len = len(items)
        excess = max(len - self.slots_left(), 0)
        self.pop_n(excess)

        

        self.end += len       

            
    def pop_n(self, n):
        if n == 0:
            return []
        
        n = min(len(self), n)

        end = (self.start + n) % self.maxlen

        if end >= self.start:
            data = self.buf[self.start:end]
        else:
            data = self.buf[self.start:].extend(self.buf[:end])

        self.start = end

        return data
    
    def slots_left(self):
        if self.end >= self.start:
            return self.end - self.start
        else:
            return self.maxlen - self.start + self.end

    def __len__(self):
        return self.maxlen - self.slots_left()

    def __str__(self):
        return "Start: {0}; End: {0}; Buf: {0}"



class DynamicPlotter():

    def __init__(self, input_buffer, sampleinterval=0.1, timewindow=10., size=(600,350)):
        # Data stuff
        self._interval = int(sampleinterval*1000)
        self._bufsize = int(timewindow/sampleinterval)
        
        self.input_buffer = input_buffer
        
        self.databuffer = collections.deque([0.0]*self._bufsize, self._bufsize)
        self.x = np.linspace(-timewindow, 0.0, self._bufsize)
        self.y = np.zeros(self._bufsize, dtype=np.float)
        # PyQtGraph stuff
        self.app = QtGui.QApplication([])
        self.plt = pg.plot(title='Dynamic Plotting with PyQtGraph')
        self.plt.resize(*size)
        self.plt.showGrid(x=True, y=True)
        self.plt.setLabel('left', 'amplitude', 'V')
        self.plt.setLabel('bottom', 'time', 's')
        self.curve = self.plt.plot(self.x, self.y, pen=(255,0,0))
        # QTimer
        self.timer = QtCore.QTimer()
        self.timer.timeout.connect(self.updateplot)
        self.timer.start(self._interval)

    def getdata(self):
        data = [1, 2, 3, 4, 5]
        return data

        # frequency = 0.5
        # noise = random.normalvariate(0., 1.)
        # new = 10.*math.sin(time.time()*frequency*2*math.pi) + noise
        # return new

    def updateplot(self):
        self.databuffer.extend( self.getdata() )
        self.y[:] = self.databuffer
        self.curve.setData(self.x, self.y)
        self.app.processEvents()

    def run(self):
        self.app.exec_()

if __name__ == '__main__':
    input_buffer = InputBuffer()
    m = DynamicPlotter(input_buffer, sampleinterval=0.05, timewindow=10.)
    m.run()