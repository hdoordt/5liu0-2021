from typing import Iterable
from pyqtgraph.Qt import QtGui, QtCore
import pyqtgraph as pg

import collections

import numpy as np

import folley
import time

class DynamicPlotter():

    def __init__(self, sampleinterval=0.5, timewindow=10., size=(600,350)):
        # Data stuff
        self._interval = int(sampleinterval*1000)
        self._bufsize = int(timewindow/sampleinterval)
        self.databuffers = []
        self.databuffers.append(collections.deque([0.0]*self._bufsize, self._bufsize))
        self.databuffers.append(collections.deque([0.0]*self._bufsize, self._bufsize))
        self.databuffers.append(collections.deque([0.0]*self._bufsize, self._bufsize))
        self.databuffers.append(collections.deque([0.0]*self._bufsize, self._bufsize))

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
        data = folley.get_samples()
        print(data[0])
        return data

    def updateplot(self):
        data = self.getdata()
        for i in range(0, len(data)):
            self.databuffers[i].extend(data[i])
        
        self.y[:] = self.databuffers[0]
        self.curve.setData(self.x, self.y)
        self.app.processEvents()

    def run(self):
        self.app.exec_()

if __name__ == '__main__':
    folley.init('/dev/ttyACM0', 4)
    m = DynamicPlotter(sampleinterval=0.02, timewindow=20.)
    m.run()