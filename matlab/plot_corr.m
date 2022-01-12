function plot_corr(ref, sig, Ts, len, skip)
    if ~exist('len', 'var')
        len = -1;
    end

    if ~exist('skip', 'var')
        skip = 0;
    end

    corr = corr_samples(ref, sig, Ts, len, skip);
    plot(corr.lags, corr.corr);
    xlabel('Offset ()');
    ylabel('Magnitude');
    title('Cross correlation');
end