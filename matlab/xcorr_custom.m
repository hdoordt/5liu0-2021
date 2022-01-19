function c = xcorr_custom(ref, sig, Ts)
    if (length(ref) * 2 - 1) > length(sig)
        % signal should be 3 times the size of the reference 
        lref = length(ref);
        lsig = length(sig);
        error(sprintf('Length of sig (%d) should be at least three times the length of the reference (%d)', lref, lsig));
    end
    
    N = length(ref);
    

    corr = zeros(1, 2*N+1);
    lags = linspace(-N, N, 2*N + 1);
    for n = linspace(1, 2*N, 2*N)
        for m = linspace(1, N, N)
            corr(n) = corr(n) + ref(m) * conj(sig(n + m));
        end
    end
    
    [~, lag] = max(corr);
    lag = lag - N - 1;
    c.lags = lags;
    c.tau = lag*Ts;
    % c.tau = -0.125/343;
    % c.tau/Ts
    c.lag = lag;
    c.corr = corr;
    
end