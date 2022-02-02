function corr = corr_samples(ref, sig, Ts)    
    m = length(ref);
    
    n = linspace(-m, m, m*2-1);
    [res, lags] = xcorr(ref, sig);
     
    corr.t = n * Ts;
    corr.n = n;
    corr.corr = res;
    corr.lags = lags;
end
