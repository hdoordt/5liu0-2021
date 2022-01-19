function theta = calc_angle(ref, sig, Ts)
    % corr = corr_samples(ref, sig, Ts);
    % [~, argmax] = max(corr.corr);
    % lag = corr.lags(argmax);
    % tau = lag * Ts; 
    c = xcorr_custom(ref, sig, Ts);
    tau = c.tau
    v = 343; % Speed of sound
    d_mics = 0.125; % Distance between the two microphones
    cos_theta = tau * v / d_mics
    theta = acos(cos_theta);
end