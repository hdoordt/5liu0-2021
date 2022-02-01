function plot_corr(ref, sig, Ts)
    corr = corr_samples(ref, sig, Ts);
    plot(corr.lags, corr.corr);
    xlabel('Offset ()');
    ylabel('Magnitude');
    title('Cross correlation');
end