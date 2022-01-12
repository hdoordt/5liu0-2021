from pyqtgraph.Qt import QtGui, QtCore
import pyqtgraph as pg

import collections

import numpy as np

import folley
import sys
import signal


def handler(signum, frame):
    print("exit")
    sys.exit(0)


class DynamicPlotter():

    def __init__(self, sampleinterval=0.5, timewindow=10., size=(600,350), cr = 2):
        self.cr = cr
        folley.init('/dev/ttyACM0', cr)
        # Data stuff
        self._interval = int(sampleinterval*1000)
        self._bufsize = int(timewindow/sampleinterval)
        
        # PyQtGraph stuff
        self.app = QtGui.QApplication([])
        self.plt = pg.plot(title='Raw microphone samples')
        self.plt.resize(*size)
        self.plt.showGrid(x=True, y=True)
        self.plt.setLabel('left', 'amplitude', 'V')
        self.plt.setLabel('bottom', 'samples', 'n')
        
        self.databuffers = []
        self.curves = []
        self.y = []
        self.x = np.linspace(-timewindow, 0.0, self._bufsize)
        self.sps_track = collections.deque(maxlen=500)
        
        colors = [(255,0,0), (0,255,0), (0,0,255), (255,255,0)]
        for i in range(0, 4):
            self.databuffers.append(collections.deque([0.0]*self._bufsize, self._bufsize))
            self.y.append(np.zeros(self._bufsize, dtype=np.float))
            self.curves.append(self.plt.plot(self.x, self.y[i], pen=colors[i]))
               

        # QTimer
        self.timer = QtCore.QTimer()
        self.timer.timeout.connect(self.updateplot)
        self.timer.start(self._interval)

    def getdata(self):
        data = folley.get_samples()
        self.sps_track.append(len(data[0]))
        return data

    def sps(self):
        return self.cr * 1000 * sum(self.sps_track) / (len(self.sps_track) * self._interval)

    def updateplot(self):
        data = self.getdata()
        for i in range(0, len(data)):
            self.databuffers[i].extend(data[i])
            self.y[i][:] = self.databuffers[i]
            self.curves[i].setData(self.x, self.y[i])
        # print(self.sps())
        self.app.processEvents()

    def run(self):
        self.app.exec_()

if __name__ == '__main__':

    signal.signal(signal.SIGINT, handler)

    print(sys.argv)
    m = DynamicPlotter(sampleinterval=0.02, timewindow=1000., cr=1)
    m.run()