function fft_out = fft_channel(x, Ts)
    y = fft(x);
    fs = 1/Ts;
    f = (0:length(y)-1)*fs/length(y)/2; % No idea why we need to divide by 2 here (maybe because of nyquist freq?)

    fft_out.f = f;
    fft_out.y = y;
end

