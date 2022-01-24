function fft_out = fft_channel(x, Ts)
    y = fft(x);
    N = length(y);
    f = (0:N-1)/Ts/N;

    fft_out.f = f;
    fft_out.y = y;
end

