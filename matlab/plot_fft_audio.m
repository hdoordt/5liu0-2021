% Plot audio file Fourier transform
% Adapted from https://nl.mathworks.com/matlabcentral/fileexchange/60370-audio-fourier-transform
function plot_fft_audio(file_path)
    [~, name, ext] = fileparts(file_path);

    [x, Fs] = audioread(file_path);
    nf=size(x,1);
    Y = fft(x-mean(x))/nf;
    f = Fs/2*linspace(0,1,fix(nf/2)+1);
    figure
    plot(f,abs(Y(1:nf/2+1))*2)
    xlim([0 1.5E+4])
    set(gca, 'XTick', (0:2500:15000))
    xlabel('Frequency (Hz)'); ylabel('Magnitude');
    title(sprintf('Frequency distribution of %s%s',name,ext));
end