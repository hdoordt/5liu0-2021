function theta = calc_angle(ref, sig, Ts)
    corr = corr_samples(ref, sig);
    [~, argmax] = max(corr.corr);
    lag = corr.lags(argmax);
    theta = lag_to_angle(lag, Ts, 0.125);
end