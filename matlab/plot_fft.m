function plot_fft(x, Ts)
    fft_out = fft_channel(x, Ts);
    plot(fft_out.f, abs(fft_out.y));
    xlabel('Frequency (Hz)')
    ylabel('Magnitude')
    title('FFT plot')
end